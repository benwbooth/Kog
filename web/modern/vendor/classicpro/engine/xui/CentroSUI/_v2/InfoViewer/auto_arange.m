/*---------------------------------------------------
-----------------------------------------------------
Filename:	auto_arange.m
Version:	1.0

Type:		maki
Date:		19. Oct. 2012 - 01:54PM 
Author:		Pieter Nieuwoudt aka pjn123
E-Mail:		pjn123@outlook.com
Internet:	www.skinconsortium.com
-----------------------------------------------------
---------------------------------------------------*/

#include <lib/std.mi>
#define space 2

Function doArangeNow();
Function doResizeNow();
Function int getNumOfRows();

Global Group g;
Global int num_objects, num_of_rows, obj_width, current_row, x_pos, next_y, next_x, calc_w, calc2_w, next_w,last_refresh_rows, skip, new_w;
Global boolean busy;
Global GuiObject tagHandle;
Global Text tagHandleText;
Global String s1, s2;

Global GuiObject obj1, obj2, custom;
Global Text text1;

System.onScriptLoaded(){

	g = getScriptGroup();
	num_objects = g.getNumObjects();
	last_refresh_rows = -100;
	busy = false;
	

}

g.onSetVisible(boolean onOff){
	if(onOff){
		busy = true;
		//doArangeNow();
		doResizeNow();
		busy = false;
	}
}

g.onresize(int x, int y, int w, int h){
	if(!busy && last_refresh_rows != -100 && g.isVisible() && last_refresh_rows != getNumOfRows() && g.isVisible()) doArangeNow(); //Limit the refreshes!
}

int getNumOfRows(){
	custom = g.enumObject(0);
	
	for(int a = 0; a<num_objects; a++){
		custom = g.enumObject(a);
		if(custom.isVisible()) break;
	}
	
	num_of_rows = (g.getHeight()-3)/custom.getHeight();
	return num_of_rows;
}

doResizeNow(){
	GuiObject custom2;
	
	new_w = getPublicInt("cpro2.tags.width", 300);
	
	for(int a = 0; a<num_objects; a++){
		custom2 = g.enumObject(a);
		custom2.setXmlParam("w", integerToString(new_w));
	}
	
	delete custom2;
	if(g.isVisible()) doArangeNow();
}

doArangeNow(){
	if(getPublicInt("cpro2.tags.50", 1)) g.setAlpha(0); //Text overlap when track change for a split second
	//Show/hide tags per user settings
	for(int b = 100; b<116;b++){
		tagHandle = g.getObject("cpro2.tags." + integerToString(b));
		tagHandleText = tagHandle;
		
		//debug("cpro2.tags."+integerToString(b)+" - "+integerToString(getPublicInt("cpro2.tags."+integerToString(b), 1)));
		if(getPublicInt("cpro2.tags."+integerToString(b), 1)==1){
			if(b==114) tagHandle.show();//Rating
			else if(getPublicInt("cpro2.tags.50", 1) && (tagHandleText.getText() == "" || tagHandleText.getText() == "-")){
				tagHandle.hide();
			}
			else{
				tagHandle.show();
			}
		}
		else tagHandle.hide();
	}


	obj1 = g.enumObject(0);
	//num_of_rows = g.getHeight()/obj1.getHeight();
	num_of_rows = getNumOfRows();
	//debugString(integerToString(num_of_rows),9);

	//debug("num_objects=" + integerToString(num_objects) + "  g.height=" + integerToString(g.getHeight()) + "  obj.height=" + integerToString(obj1.getHeight()));

	obj_width = obj1.getWidth() + space;

	//debugint(num_of_rows);

	current_row = 0;
	next_y = 2;
	
	next_x = 0;
	next_w = 0;
	calc2_w = 0;
	calc_w = 0;
	
	skip = 0;
	
	if(obj1.getHeight() * num_of_rows > g.getHeight()) num_of_rows--; //There's no round down function in maki :(
	
	//arange first object
	obj1 = g.enumObject(0);
	
	if(!obj1.isVisible()) skip--;
	
	for(int a = 0; a<num_objects-1; a++){
		obj1 = g.enumObject(a);
		obj2 = g.enumObject(a+1);
		if(obj1 == NULL || obj2 == NULL){
			debug("ERROR!!!");
		}
		
		if(!obj2.isVisible()) skip--;
		else if(getPublicInt("cpro2.tags.smallest", 1)){
			text1 = obj2.findObject("text");
			calc_w = text1.getGuiX()+text1.getTextWidth()+space;
			if(next_w < calc_w) calc2_w = calc_w;
			if(calc2_w > obj_width) calc2_w = obj_width; //limit line to maximum value
		}
		
		
		if(a+2+skip > num_of_rows*(current_row+1)){ //next row
			current_row++;
			next_y = 2;

			next_x += next_w;
			next_w=calc_w;
		}
		else{ //next line
			
			next_y = next_y+obj1.getHeight()*obj1.isVisible();
			next_w=calc2_w;

		}
		
		if(!getPublicInt("cpro2.tags.smallest", 1)) next_x = current_row*obj_width;

		obj2.setXMLParam("y", integerToString(next_y)); //maybe improve here
		obj2.setXMLParam("x", integerToString(next_x));
	}
	last_refresh_rows = num_of_rows;

	if(getPublicInt("cpro2.tags.50", 1) || g.getAlpha()!=253) g.setAlpha(253);
}

//.sendAction(String action, String param, Int x, int y, int p1, int p2);
g.onAction(String action, String param, Int x, int y, int p1, int p2, GuiObject source){
	if(strlower(action)=="update"){
		if(g.isVisible()) doArangeNow();
	}
	else if(strlower(action)=="resize_width"){
		doResizeNow();
	}
	else if(strlower(action)=="update_textsize"){
		GuiObject custom2;
		for(int a = 0; a<num_objects; a++){
			custom2 = g.enumObject(a);
			custom2.setXmlParam("textsize", ""); //don't use the value - xui object gets it from privateint value
		}
		
		delete custom2;
		if(g.isVisible()) doArangeNow();
	}
	else if(strlower(action)=="update_text_a"){
		if(getToken(strlower(param), ";", 0)== "fontsize" || param=="") return; //Block skinners from changing fontsize
		
		GuiObject custom2;
		for(int a = 0; a<num_objects; a++){
			custom2 = g.enumObject(a);
			s2 = getToken(param, ";", 1);

			//if(a==num_objects-1) s1 = getToken(param, ";", 0); //Last object is rating... just a quick fix... :P
			if(a==11) s1 = getToken(param, ";", 0); //Rating object... just a quick fix... :P
			else s1 = "cpro2a_"+getToken(param, ";", 0);
			
			custom2.setXmlParam(s1, s2); //add prefix so that system params go through too!
		}
		
		delete custom2;
		if(g.isVisible()) doArangeNow();
	}
	else if(strlower(action)=="update_text_b"){
		if(getToken(strlower(param), ";", 0)== "fontsize" || param=="") return; //Block skinners from changing fontsize
		
		GuiObject custom2;
		for(int a = 0; a<num_objects; a++){
			custom2 = g.enumObject(a);
			s2 = getToken(param, ";", 1);

			if(a==num_objects-1) break;
			else s1 = "cpro2b_"+getToken(param, ";", 0);
			
			custom2.setXmlParam(s1, s2); //add prefix so that system params go through too!
		}
		
		delete custom2;
		if(g.isVisible()) doArangeNow();
	}
}