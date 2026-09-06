/**
 * Manager for Widgets
 * Handles showing of a external window
 *
 * @author mpdeimos
 * @date 2009/04/28
 */

#include <lib/std.mi>

#define NUM_WIDGET_PLACES 3

Global Layout main_normal;
Global Layout manager_normal;
Class Group WidgetPlace;
Member String WidgetPlace.name;

Global List widgetPlaces, widgetList;
Global int done, widgetType; //0=main, 1=drawer, 2=side

Global GroupList grplst;//,  grplstN;
Global Slider vscroll;//, vscrollN;
Global String widgetPath, engine;

Global Group grpAll;//, grpNew;
//Global Button newW, allW, dld1, dld2;

Global Boolean newWidgetInstalled, hasWidgets;

System.onScriptLoaded ()
{
	main_normal = getContainer("main").getLayout("normal");
	manager_normal = getContainer("widgets.manager").getLayout("normal");
	//grpNew = manager_normal.findObject("widgets.manager.content.new");
	grpAll = manager_normal.findObject("widgets.manager.content.all");

	grplst = grpAll.findObject("grplst");
	grplst.setRedraw(1);
	//grplstN = grpNew.findObject("grplst");
	//grplstN.setRedraw(1);
	vscroll = grpAll.findObject("vscroll");
	//vscrollN = grpNew.findObject("vscroll");
	//newW = manager_normal.findObject("newW");
	//allW = manager_normal.findObject("allW");
	widgetPath = getToken(getParam(), ";", 0);
	engine = getToken(getParam(), ";", 1);
	if(engine=="") engine="cpro";
	widgetPlaces = new List;
	widgetList = new List;
	done = 0;
	newWidgetInstalled = false;
	hasWidgets = false;
	
	//dld1 = grplstN.instantiate("widgets.manager.listitem.none", 1).getObject("dld");
	//dld2 = grplst.instantiate("widgets.manager.listitem.none", 1).getObject("dld");
}

main_normal.onAction (String action, String param, int x, int y, int p1, int p2, GuiObject _source)
{
	if (strlower(action) == "widget_manager_register")
	{
		if(param=="Main Area") widgetType=0;
		else if(param=="Drawer Area") widgetType=1;
		else if(param=="Side Area") widgetType=2;
		
		
		
		//debugstring("1: "+param,9);
		complete;
		WidgetPlace wp = _source;
		wp.name = param;
		widgetPlaces.addItem(wp);
		return widgetPlaces.getNumItems()-1;
		
	}
	else if (strlower(action) == "widget_manager_check")
	{
		//debug("2: "+param);
		int widgetNo;
		boolean createNewItem=false;
		
		if(widgetList.getNumItems()==0) createNewItem = true;
		
		for(int i = 0; i< widgetList.getNumItems(); i++){
			if(widgetList.enumItem(i)==param){
				widgetNo = i+1;
				//debug(widgetList.enumItem(i) + " = " + param);
				createNewItem = false;
				break;
			}
			else if(i==widgetList.getNumItems()-1){
				createNewItem = true;
				break;
			}
			
		}
		
		Group g = NULL;
		if(createNewItem){

			widgetList.addItem(param);
			widgetNo = widgetList.getNumItems();
	
			GuiObject data = NULL;

			g = grplst.instantiate("widgets.manager.listitem", 1);
			Group source = _source;
			g.setXmlParam("widgetname", getToken(source.getXmlParam("name"), ";", 0));

			data = source.getObject("@version");
			if(data) g.setXmlParam("widgetversion", data.getXmlParam("userdata"));

			//data = NULL;
			data = source.getObject("@author");
			if(data) g.setXmlParam("widgetauthor", data.getXmlParam("userdata"));
			g.setXmlParam("widgetid", param);

			//data = NULL;
			data = source.getObject("@uninstaller");
			if(data) g.setXmlParam("widgetuninstaller", widgetPath + data.getXmlParam("userdata"));

			//data = NULL;
			data = source.getObject("@support");
			if(data) g.setXmlParam("widgetsupport", data.getXmlParam("userdata"));

			//g.setXmlParam("userdata", integertostring(x));
			
		}
		
		
		g = grplst.enumItem(widgetNo-1);
		//if(g==Null) debug("123");
		if(widgetType==0){
			g.setXmlParam("widgetpos_main",integertostring(x));
		}
		else if(widgetType==1){
			g.setXmlParam("widgetpos_drawer",integertostring(x));
		}
		else if(widgetType==2){
			g.setXmlParam("widgetpos_mini",integertostring(x));
		}
		//g.setXmlParam("userdata", integertostring(x));
		
		
		
		//*debugstring("2: "+param,9);
		WidgetPlace wp = widgetPlaces.enumItem(x);

		Group source = _source;
		GuiObject data = NULL;
		String version = "";
		data = source.getObject("@version");

		if (data != null)
			version = data.getXmlParam("userdata");

		//if (getPrivateString(engine+".widget-manager.check", integerToString(x)+param, "") != version)
		if (getPublicString(engine+".widget-manager.check;"+integerToString(x)+param,"") != version)
		{
			setPublicString(engine+".widget-manager.check;"+integerToString(x)+param, version);
			//setPrivateString(engine+".widget-manager.check", integerToString(x)+param, version);
			/*if (!newWidgetInstalled)
			{
				grplstN.removeAll();
			}*/
			
			newWidgetInstalled = true;
			g.setXmlParam("hilight", "1");
		}
			/*Group g = grplstN.instantiate("widgets.manager.listitem", 1);
			g.setXmlParam("widgetname", getToken(source.getXmlParam("name"), ";", 0));
			g.setXmlParam("widgetposition", wp.name);
			g.setXmlParam("widgetid", param);
			g.setXmlParam("userdata", integertostring(x));

			data = NULL;
			data = source.getObject("@author");
			if (data)
				g.setXmlParam("widgetauthor", data.getXmlParam("userdata"));

			g.setXmlParam("widgetversion", version);

			data = NULL;
			data = source.getObject("@uninstaller");
			if (data)
				g.setXmlParam("widgetuninstaller", widgetPath + data.getXmlParam("userdata"));

			data = NULL;
			data = source.getObject("@support");
			if (data)
				g.setXmlParam("widgetsupport", data.getXmlParam("userdata"));
		}		

		if (!hasWidgets)
		{
			grplst.removeAll();
			hasWidgets = true;
		}

		Group g = grplst.instantiate("widgets.manager.listitem", 1);
		g.setXmlParam("widgetname", getToken(source.getXmlParam("name"), ";", 0));
		g.setXmlParam("widgetposition", wp.name);
		g.setXmlParam("widgetid", param);
		g.setXmlParam("userdata", integertostring(x));

		data = source.getObject("@author");
		if (data)
			g.setXmlParam("widgetauthor", data.getXmlParam("userdata"));

		g.setXmlParam("widgetversion", version);

		data = NULL;
		data = source.getObject("@uninstaller");
		if (data)
			g.setXmlParam("widgetuninstaller", widgetPath + data.getXmlParam("userdata"));

		data = NULL;
		data = source.getObject("@support");
		if (data)
			g.setXmlParam("widgetsupport", data.getXmlParam("userdata"));
			
		*/
	}
	else if (strlower(action) == "widget_manager_done")
	{
		done++;
		if (done == NUM_WIDGET_PLACES && newWidgetInstalled == true)
		{
			//newW.onLeftClick();
			manager_normal.show();
		}
		
	}
	else if (strlower(action) == "widget_manager_show")
	{
		manager_normal.show();
	}
	else if (strlower(action) == "widget_manager_hide")
	{
		manager_normal.hide();
	}
	else if (strlower(action) == "widget_manager_visible")
	{
		complete;
		return manager_normal.isvisible();
	}
}

manager_normal.onAction (String action, String param, Int x, int y, int p1, int p2, GuiObject source)
{
	if (strlower(action) == "show_widget")
	{
		WidgetPlace wp = widgetPlaces.enumItem(x);
		wp.sendAction("show_widget", param, 0,0,0,0);
	}
	
}

vscroll.onSetPosition(int newpos)
{
	int percent = 99-newpos;
	//grplst.setXmlParam("y", integertostring(-newpos));
	grplst.scrolltopercent(percent);
	grplst.setRedraw(percent);
}

/*vscrollN.onSetPosition(int newpos)
{
	int percent = 99-newpos;
	//grplst.setXmlParam("y", integertostring(-newpos));
	grplstN.scrolltopercent(percent);
	grplstN.setRedraw(percent);
}*/

/*newW.onLeftClick ()
{
	grpAll.hide();
	grpNew.show();
}

allW.onLeftClick ()
{
	grpNew.hide();
	grpAll.show();
}*/

/*dld1.onLeftClick ()
{
	System.navigateURL("http://www.skinconsortium.com/index.php?page=Downloads&typeID=7");
}

dld2.onLeftClick ()
{
	System.navigateURL("http://www.skinconsortium.com/index.php?page=Downloads&typeID=7");
}*/
