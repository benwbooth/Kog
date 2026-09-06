/*************************************************************

	shadow.m file
	classicPro add-on shadow scripts
	
	by Leechbite
	
	param = "containerID,layoutID,shadowContID,snapLeft,snapRight,snapTop,snapLeft,offsetX,offsetY,offsetW,offsetH"
	
	containerID - id of main container;
	layoutID - id of main layout;
	shadowContID - id of shadow container;
	snapLeft,snapTop,snapRight,snapBottom - used snapAdjust the main layout;
	offsetX,offsetY,offsetW,offsetH - added(or subtracted) to main layout coordinates for the shadow coordinates;
  
*************************************************************/

#include <lib/std.mi>

Global container shadowCont;
Global layout mainLayout, shadowLayout;
Global string contID, layID, shadowContID;
Global int snapLeft,snapRight,snapTop,snapBottom,offsetX,offsetY,offsetW,offsetH;
Global int onLoad = false;

System.onScriptLoaded() {

	string param = getParam();

	
	contID = getToken(param,",",0);
	layID =  getToken(param,",",1);
	shadowContID =  getToken(param,",",2);

	snapLeft = stringToInteger(getToken(param,",",3));
	snapTop = stringToInteger(getToken(param,",",4));
	snapRight = stringToInteger(getToken(param,",",5));
	snapBottom = stringToInteger(getToken(param,",",6));
	offsetX = stringToInteger(getToken(param,",",7));
	offsetY = stringToInteger(getToken(param,",",8));
	offsetW = stringToInteger(getToken(param,",",9));
	offsetH = stringToInteger(getToken(param,",",10));
	
	container mainCont = getContainer(contID);
	if (maincont) mainLayout = mainCont.getLayout(layID);
	
	if (mainLayout) {
		mainLayout.snapAdjust(snapLeft,snapRight,snapTop,snapBottom);
		if (mainLayout.isVisible() && !onLoad) {
			mainLayout.onSetVisible(1);
		}
	}
	
	
}

System.onScriptUnloading() {
	return;
}

mainLayout.onSetVisible(int on) {
	if (on) {
		onLoad = true;
		shadowCont = newDynamicContainer(shadowContID);
		
		if (shadowCont) shadowLayout = shadowCont.getLayout("normal");
		if (shadowLayout) {
			shadowLayout.show();
			mainLayout.onResize(mainLayout.getLeft(),mainLayout.getTop(),mainLayout.getWidth(),mainLayout.getHeight());
		}
		
	} else {
		if (shadowCont) {
			shadowCont.close();
			shadowCont = NULL;
			shadowLayout = NULL;
		}
	}
}

mainLayout.onResize(int x, int y, int w, int h) {
	
	if ((w == getViewPortWidth()) && (h == getViewPortHeight())) {
		if (shadowCont) {
			shadowCont.close();
			shadowCont = NULL;
			shadowLayout = NULL;
		}
		
		return;
	}
	
	if ((!shadowLayout) && (mainLayout.isVisible())) {
		shadowCont = newDynamicContainer("main.shadow");
		
		if (shadowCont) shadowLayout = shadowCont.getLayout("normal");
		if (shadowLayout) 
			shadowLayout.show();
	}
	
	if (!shadowLayout) return;
	
	shadowLayout.resize(getLeft() + offsetX, getTop() + offsetY, w + offsetW, h + offsetH);
}

mainLayout.onMove() {
	if (!shadowLayout) return;
	
	shadowLayout.resize(getLeft() + offsetX, getTop() + offsetY, mainLayout.getWidth() + offsetW, mainLayout.getHeight() + offsetH);
}


